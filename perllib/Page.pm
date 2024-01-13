# Copyright 2023 by Christian Jaeger <ch@christianjaeger.ch>
# Published under the same terms as perl itself
#

=head1 NAME

Page

=head1 SYNOPSIS

=head1 DESCRIPTION

=head1 SEE ALSO

=head1 NOTE

This is alpha software! Read the status section in the package README
or on the L<website|http://functional-perl.org/>.

=cut


package Page;
use strict;
use utf8;
use warnings;
use warnings FATAL => 'uninitialized';
#use experimental 'signatures';
use Exporter "import";

our @EXPORT=qw(page);
our @EXPORT_OK=qw();
our %EXPORT_TAGS=(default => \@EXPORT, all=>[@EXPORT,@EXPORT_OK]);

use FP::Show; use FP::Repl; use FP::Repl::Trap; #
use PXML::XHTML ":all";


sub page {
    HTML(HEAD(),
         BODY())
}



1
